using System;

namespace KairosEngine.Graphics
{
	extension GraphicsShader
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsShader_Dispose")]
		private static extern void GraphicsShader_Dispose(GraphicsShader* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsShader_CreateWithoutErrorInfo")]
		private static extern int32 GraphicsShader_CreateWithoutErrorInfo(GraphicsShader* _this, char16* path, ShaderType type, ShaderCompileFlags compileFlags);
	}
}