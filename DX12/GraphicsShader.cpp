#include "GraphicsShader.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsShader_Dispose(GraphicsShader* _this)
{
	SAFE_RELEASE(_this->m_pShader);
}

int KAIROS_API GraphicsShader_CreateWithoutErrorInfo(GraphicsShader* _this, LPCWSTR path, ShaderType type, UINT compileFlags)
{
	ID3DBlob* pShader;
	LPCSTR target;
	switch (type)
	{
	case VertexShader:
		target = "vs_5_0";
		break;
	case FragmentShader:
		target = "ps_5_0";
		break;
	default:
		return CreateShaderFailed;
	}
	HRESULT hr = D3DCompileFromFile(
		path,
		nullptr, nullptr,
		"main", target,
		compileFlags,
		0,
		&pShader,
		nullptr
	);
	if (FAILED(hr))
		return CreateShaderFailed;

	_this->m_pShader = pShader;
	return GraphicsSuccess;
}

KAIROS_EXPORT_END