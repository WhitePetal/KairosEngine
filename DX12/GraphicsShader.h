#ifndef __KAIROS_GRAPHICS_SHADER__
#define __KAIROS_GRAPHICS_SHADER__

#include "ErrorCodes.h"
#include "KairosEngineDefines.h"
#include "ShaderType.h"
#include <d3d12.h>
#include <dxgi1_4.h>
#include <D3Dcompiler.h>
#include <DirectXMath.h>
#include "d3dx12.h"

typedef struct GraphicsShader
{
	ID3DBlob* m_pShader;
} GraphicsShader;

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsShader_Dispose(GraphicsShader* _this);

int KAIROS_API GraphicsShader_CreateWithoutErrorInfo(GraphicsShader* _this, LPCWSTR path, ShaderType type, UINT compileFlags);

KAIROS_EXPORT_END

#endif
