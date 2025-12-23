#ifndef __KAIROS_GRAPHICS_PIPELINE_STATE__
#define __KAIROS_GRAPHICS_PIPELINE_STATE__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "ShaderType.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsPipelineState
{
	ID3D12PipelineState* m_pPipelineState;
} GraphicsPipelineState;


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsPipelineState_Dispose(GraphicsPipelineState* _this);

KAIROS_EXPORT_END

#endif