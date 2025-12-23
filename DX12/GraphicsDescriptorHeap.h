#ifndef __KAIROS_GRAPHICS_DESCRIPTOR_HEAP__
#define __KAIROS_GRAPHICS_DESCRIPTOR_HEAP__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsDescriptorHeap
{
	ID3D12DescriptorHeap* m_pDescriptorHeap;

	D3D12_DESCRIPTOR_HEAP_TYPE m_Type;
	D3D12_DESCRIPTOR_HEAP_FLAGS m_Flags;
} GraphicsDescriptorHeap;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDescriptorHeap_Dispose(GraphicsDescriptorHeap* _this);

SIZE_T KAIROS_API GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(GraphicsDescriptorHeap* _this);

KAIROS_EXPORT_END

#endif // !__KAIROS_GRAPHICS_DESCRIPTOR_HEAP__
