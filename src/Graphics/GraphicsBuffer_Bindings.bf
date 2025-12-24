using System;

namespace KairosEngine.Graphics
{
	extension GraphicsBuffer
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsBuffer_Dispose")]
		private static extern void GraphicsBuffer_Dispose(GraphicsBuffer* _this);
	}
}