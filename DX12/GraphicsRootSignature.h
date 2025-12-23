#ifndef __KAIROS_GRAPHICS_ROOT_SIGNATURE__
#define __KAIROS_GRAPHICS_ROOT_SIGNATURE__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "CreateResult.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsRootSignature
{
	ID3D12RootSignature* m_pRootSignature;
} GraphicsRootSignature;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsRootSignature_Dispose(GraphicsRootSignature* _this);

KAIROS_EXPORT_END

#endif