#include "GraphicsFence.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsFence_Dispose(ID3D12Fence* _this)
{
	_this->Release();
}

UINT64 KAIROS_API GraphicsFence_GetCompletedValue(ID3D12Fence* _this)
{
	return _this->GetCompletedValue();
}

int KAIROS_API GraphicsFence_SetEventOnCompletion(ID3D12Fence* _this, UINT64 fenceValue, HANDLE* pFenceEvent)
{
	HRESULT hr = _this->SetEventOnCompletion(fenceValue, pFenceEvent);
	return hr;
}

KAIROS_EXPORT_END