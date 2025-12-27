#include "GraphicsResource.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsResource_Dispose(ID3D12Resource* _this)
{
	_this->Release();
}

UINT64 KAIROS_API GraphicsResource_GetGPUVirtualAddress(ID3D12Resource* _this)
{
	return _this->GetGPUVirtualAddress();
}

void KAIROS_API GraphicsResource_Unmap(ID3D12Resource* _this, UINT subResource, UINT64 begin, UINT64 end)
{
	D3D12_RANGE range = { begin, end };
	_this->Unmap(subResource, &range);
}

KAIROS_EXPORT_END