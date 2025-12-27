#ifndef __KAIROS_GRAPHICS_PIPELINE_STATE__
#define __KAIROS_GRAPHICS_PIPELINE_STATE__

#include "KairosEngineDefines.h"
#include "ShaderType.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"


KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsPipelineState_Dispose(ID3D12PipelineState* _this);

KAIROS_EXPORT_END

#endif