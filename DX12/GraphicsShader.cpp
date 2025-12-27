#include "GraphicsShader.h"

KAIROS_EXPORT_BEGIN

void KAIROS_API GraphicsShader_Dispose(ID3DBlob* _this)
{
	_this->Release();
}

int KAIROS_API GraphicsShader_CreateWithoutErrorInfo(ID3DBlob** p_this, LPCWSTR path, ShaderType type, UINT compileFlags)
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
		return 1;
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
		return hr;

	*p_this = pShader;
	return hr;
}

KAIROS_EXPORT_END