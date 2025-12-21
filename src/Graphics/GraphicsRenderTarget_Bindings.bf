using System;

namespace KairosEngine.Graphics
{
	extension GraphicsRenderTarget
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsRenderTarget_Dispose")]
		private static extern void GraphicsRenderTarget_Dispose(void* _this);
	}
}