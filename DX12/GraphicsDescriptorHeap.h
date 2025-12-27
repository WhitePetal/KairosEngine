#ifndef __KAIROS_GRAPHICS_DESCRIPTOR_HEAP__
#define __KAIROS_GRAPHICS_DESCRIPTOR_HEAP__

#include "KairosEngineDefines.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDescriptorHeap_Dispose(ID3D12DescriptorHeap* _this);

D3D12_DESCRIPTOR_HEAP_DESC KAIROS_API GraphicsDescriptorHeap_GetDesc(ID3D12DescriptorHeap* _this);

D3D12_CPU_DESCRIPTOR_HANDLE KAIROS_API GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(ID3D12DescriptorHeap* _this);

D3D12_GPU_DESCRIPTOR_HANDLE KAIROS_API GraphicsDescriptorHeap_GetGPUDescriptorHandleForHeapStart(ID3D12DescriptorHeap* _this);

KAIROS_EXPORT_END

#endif // !__KAIROS_GRAPHICS_DESCRIPTOR_HEAP__
