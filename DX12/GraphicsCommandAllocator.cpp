#include "GraphicsCommandAllocator.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsCommandAllocator_Dispose(ID3D12CommandAllocator* _this)
{
	_this->Release();
}

int KAIROS_API GraphicsCommandAllocator_Reset(ID3D12CommandAllocator* _this)
{
	return _this->Reset();
}

KAIROS_EXPORT_END