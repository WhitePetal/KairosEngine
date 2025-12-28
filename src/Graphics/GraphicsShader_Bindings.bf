using System;

namespace KairosEngine.Graphics
{
	extension GraphicsShader
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsShader_Dispose")]
		private static extern void GraphicsShader_Dispose(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsShader_CreateWithoutErrorInfo")]
		private static extern int32 GraphicsShader_CreateWithoutErrorInfo(void** p_this, char16* path, ShaderType type, ShaderCompileFlags compileFlags);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsShader_GetBufferPointer")]
		private static extern void* GraphicsShader_GetBufferPointer(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsShader_GetBufferSize")]
		private static extern uint64 GraphicsShader_GetBufferSize(void* _this);
	}
}