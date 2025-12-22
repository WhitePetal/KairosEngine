#ifndef __KAIROS_GRAPHICS_COMMAND_ALLOCATOR__
#define __KAIROS_GRAPHICS_COMMAND_ALLOCATOR__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsCommandAllocator
{
public:
	GraphicsCommandAllocator(ID3D12CommandAllocator* pCommandAllocator);
	
	void Dispose();

	inline ID3D12CommandAllocator* GetInternalPtr();

private:
	ID3D12CommandAllocator* m_pCommandAllocator;
};

inline ID3D12CommandAllocator* GraphicsCommandAllocator::GetInternalPtr()
{
	return m_pCommandAllocator;
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(GraphicsCommandAllocator* _this);

KAIROS_EXPORT_END

#endif
