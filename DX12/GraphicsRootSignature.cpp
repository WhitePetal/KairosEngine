#include "GraphicsRootSignature.h"

void KAIROS_API GraphicsRootSignature_Dispose(GraphicsRootSignature* _this)
{
	SAFE_RELEASE(_this->m_pRootSignature);
}