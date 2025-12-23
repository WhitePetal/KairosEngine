#include "GraphicsRootSignature.h"

GraphicsRootSignature::GraphicsRootSignature(ID3D12RootSignature* pRootSignature)
{
	m_pRootSignature = pRootSignature;
}

void GraphicsRootSignature::Dispose()
{
	SAFE_RELEASE(m_pRootSignature);
}

void KAIROS_API GraphicsRootSignature_Dispose(GraphicsRootSignature* _this)
{
	_this->Dispose();
	delete _this;
}