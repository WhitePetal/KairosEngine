#ifndef __KAIROS_GRAPHICS_RENDER_TARGET__
#define __KAIROS_GRAPHICS_RENDER_TARGET__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

class GraphicsRenderTarget
{
public:
	GraphicsRenderTarget(ID3D12Resource* pRenderTarget);

	void Dispose();

	inline ID3D12Resource* GetInternalPtr();

private:
	ID3D12Resource* m_pRenderTarget;

};

ID3D12Resource* GraphicsRenderTarget::GetInternalPtr()
{
	return m_pRenderTarget;
}

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRenderTarget_Dispose(GraphicsRenderTarget* _this);

KAIROS_EXPORT_END

#endif
