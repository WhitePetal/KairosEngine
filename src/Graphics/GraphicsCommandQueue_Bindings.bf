using System;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandQueue
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandQueue_Dispose")]
		public static extern void GraphicsCommandQueue_Dispose(void* _this);
	}
}