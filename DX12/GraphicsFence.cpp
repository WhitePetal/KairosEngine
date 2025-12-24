#include "GraphicsFence.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsFence_Dispose(GraphicsFence* _this)
{
	SAFE_RELEASE(_this->m_pFence);
}

UINT64 KAIROS_API GraphicsFence_GetCompletedValue(GraphicsFence* _this)
{
	return _this->m_pFence->GetCompletedValue();
}

int KAIROS_API GraphicsFence_SetEventOnCompletion(GraphicsFence* _this, UINT64 fenceValue, GraphicsFenceEvent* pGraphicsFenceEvent)
{
	HRESULT hr = _this->m_pFence->SetEventOnCompletion(fenceValue, pGraphicsFenceEvent->m_Handle);
	if (FAILED(hr))
		return FenceSetCompletionEventFailed;

	return GraphicsSuccess;
}

KAIROS_EXPORT_END