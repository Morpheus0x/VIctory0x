//! The presented picture kept to its own shape: the back buffer is the
//! window's client area, and the frame is drawn into the largest rectangle
//! of that area with the frame's aspect, the rest filled with a grey.
//! So a resized window, and the borderless fullscreen the title switches to
//! with Ctrl+Enter, letterbox or pillarbox instead of stretching.

use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::{PCSTR, s};

use crate::host::Host;

const SOURCE: &str = r#"
#define RS "RootConstants(num32BitConstants=2, b0, visibility=SHADER_VISIBILITY_PIXEL), DescriptorTable(SRV(t0, numDescriptors=1), visibility=SHADER_VISIBILITY_PIXEL), StaticSampler(s0, filter=FILTER_MIN_MAG_MIP_LINEAR, addressU=TEXTURE_ADDRESS_CLAMP, addressV=TEXTURE_ADDRESS_CLAMP)"
Texture2D<float4> scene : register(t0);
SamplerState smooth_ : register(s0);
cbuffer fit : register(b0) { float2 target; };
struct V { float4 at : SV_Position; };
[RootSignature(RS)]
V vs(uint id : SV_VertexID) {
    V v;
    float2 uv = float2((id << 1) & 2, id & 2);
    v.at = float4(uv * float2(2, -2) + float2(-1, 1), 0, 1);
    return v;
}
[RootSignature(RS)]
float4 ps(V v) : SV_Target {
    uint w, h;
    scene.GetDimensions(w, h);
    float2 image = max(float2(w, h), 1.0);
    float scale = min(target.x / image.x, target.y / image.y);
    float2 shown = image * scale;
    float2 origin = floor((target - shown) * 0.5);
    float2 p = v.at.xy - origin;
    if (p.x < 0 || p.y < 0 || p.x >= shown.x || p.y >= shown.y) {
        return float4(0.08, 0.08, 0.08, 1);
    }
    return float4(scene.SampleLevel(smooth_, p / shown, 0).rgb, 1);
}
"#;

pub(crate) struct Fit {
    root: ID3D12RootSignature,
    pso: ID3D12PipelineState,
    views: Vec<ID3D12DescriptorHeap>,
}

// SAFETY: D3D12 objects are free-threaded.
unsafe impl Send for Fit {}
// SAFETY: as above.
unsafe impl Sync for Fit {}

fn compile(entry: PCSTR, target: PCSTR) -> windows::core::Result<ID3DBlob> {
    let mut code: Option<ID3DBlob> = None;
    // SAFETY: the source and names live through the call; the out pointer is
    // ours.
    unsafe {
        D3DCompile(
            SOURCE.as_ptr().cast(),
            SOURCE.len(),
            s!("fit"),
            None,
            None,
            entry,
            target,
            0,
            0,
            &mut code,
            None,
        )?;
    }
    code.ok_or_else(windows::core::Error::empty)
}

impl Fit {
    pub(crate) fn new(host: &Host, format: DXGI_FORMAT) -> windows::core::Result<Self> {
        let (vs, ps) = (
            compile(s!("vs"), s!("vs_5_1"))?,
            compile(s!("ps"), s!("ps_5_1"))?,
        );
        // SAFETY: the blobs' own bytes, alive through the calls.
        unsafe {
            let bytes = |b: &ID3DBlob| {
                std::slice::from_raw_parts(b.GetBufferPointer().cast::<u8>(), b.GetBufferSize())
            };
            let device = &host.device;
            let root: ID3D12RootSignature = device.CreateRootSignature(0, bytes(&vs))?;
            let code = |b: &ID3DBlob| D3D12_SHADER_BYTECODE {
                pShaderBytecode: b.GetBufferPointer(),
                BytecodeLength: b.GetBufferSize(),
            };
            let mut desc = D3D12_GRAPHICS_PIPELINE_STATE_DESC {
                pRootSignature: std::mem::transmute_copy(&root),
                VS: code(&vs),
                PS: code(&ps),
                SampleMask: u32::MAX,
                RasterizerState: D3D12_RASTERIZER_DESC {
                    FillMode: D3D12_FILL_MODE_SOLID,
                    CullMode: D3D12_CULL_MODE_NONE,
                    ..Default::default()
                },
                PrimitiveTopologyType: D3D12_PRIMITIVE_TOPOLOGY_TYPE_TRIANGLE,
                NumRenderTargets: 1,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                ..Default::default()
            };
            desc.RTVFormats[0] = format;
            desc.BlendState.RenderTarget[0].RenderTargetWriteMask =
                D3D12_COLOR_WRITE_ENABLE_ALL.0 as u8;
            let pso = device.CreateGraphicsPipelineState(&desc)?;
            Ok(Fit {
                root,
                pso,
                views: Vec::new(),
            })
        }
    }

    /// Each presenter command-list slot owns its own SRV, as the swap chain
    /// may have several presents in the air.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record(
        &mut self,
        host: &Host,
        slot: usize,
        list: &ID3D12GraphicsCommandList,
        scene: &ID3D12Resource,
        format: DXGI_FORMAT,
        back: &ID3D12Resource,
        (width, height): (u32, u32),
    ) -> windows::core::Result<()> {
        while self.views.len() <= slot {
            let heap = unsafe {
                host.device
                    .CreateDescriptorHeap(&D3D12_DESCRIPTOR_HEAP_DESC {
                        Type: D3D12_DESCRIPTOR_HEAP_TYPE_CBV_SRV_UAV,
                        NumDescriptors: 1,
                        Flags: D3D12_DESCRIPTOR_HEAP_FLAG_SHADER_VISIBLE,
                        NodeMask: 0,
                    })?
            };
            self.views.push(heap);
        }
        let views = &self.views[slot];
        // SAFETY: an open list; the view is made in a slot no frame in flight
        // reads; live textures.
        unsafe {
            let cpu = views.GetCPUDescriptorHandleForHeapStart();
            let gpu = views.GetGPUDescriptorHandleForHeapStart();
            let desc = D3D12_SHADER_RESOURCE_VIEW_DESC {
                Format: format,
                ViewDimension: D3D12_SRV_DIMENSION_TEXTURE2D,
                Shader4ComponentMapping: D3D12_DEFAULT_SHADER_4_COMPONENT_MAPPING,
                Anonymous: D3D12_SHADER_RESOURCE_VIEW_DESC_0 {
                    Texture2D: D3D12_TEX2D_SRV {
                        MipLevels: 1,
                        ..Default::default()
                    },
                },
            };
            host.device.CreateShaderResourceView(Some(scene), Some(&desc), cpu);
            let rtv = host.rtv(Some(back), None);
            list.OMSetRenderTargets(1, Some(&rtv), false, None);
            list.SetDescriptorHeaps(&[Some(views.clone())]);
            list.SetGraphicsRootSignature(&self.root);
            list.SetPipelineState(&self.pso);
            list.SetGraphicsRoot32BitConstants(
                0,
                2,
                [width as f32, height as f32].as_ptr().cast(),
                0,
            );
            list.SetGraphicsRootDescriptorTable(1, gpu);
            list.IASetPrimitiveTopology(
                windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            );
            list.RSSetViewports(&[D3D12_VIEWPORT {
                Width: width as f32,
                Height: height as f32,
                MaxDepth: 1.0,
                ..Default::default()
            }]);
            list.RSSetScissorRects(&[windows::Win32::Foundation::RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            }]);
            list.DrawInstanced(3, 1, 0, 0);
        }
        Ok(())
    }
}
