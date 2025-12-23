using System;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandAllocator
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandAllocator_Dispose")]
		private static extern void GraphicsCommandAllocator_Dispose(GraphicsCommandAllocator* _this);
	}
}