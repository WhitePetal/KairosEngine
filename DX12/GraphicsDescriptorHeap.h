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

class GraphicsDescriptorHeap
{
public:
	GraphicsDescriptorHeap(ID3D12DescriptorHeap* pDescriptorHeap);

	void Dispose();

	inline ID3D12DescriptorHeap* GetInternalPtr();

	SIZE_T GetCPUDescriptorHandleForHeapStart();

private:
	ID3D12DescriptorHeap* m_pDescriptorHeap;
};

inline ID3D12DescriptorHeap* GraphicsDescriptorHeap::GetInternalPtr()
{
	return m_pDescriptorHeap;
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsDescriptorHeap_Dispose(GraphicsDescriptorHeap* _this);

SIZE_T KAIROS_API GraphicsDescriptorHeap_GetCPUDescriptorHandleForHeapStart(GraphicsDescriptorHeap* _this);

KAIROS_EXPORT_END

#endif // !__KAIROS_GRAPHICS_DESCRIPTOR_HEAP__
