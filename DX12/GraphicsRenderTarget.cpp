#include "GraphicsRenderTarget.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRenderTarget_Dispose(ID3D12Resource* _this)
{
	_this->Release();
}

KAIROS_EXPORT_END