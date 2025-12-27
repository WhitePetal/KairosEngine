#include "GraphicsRootSignature.h"

void KAIROS_API GraphicsRootSignature_Dispose(ID3D12RootSignature* _this)
{
	_this->Release();
}