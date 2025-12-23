using System;

namespace KairosEngine.Graphics
{
	extension GraphicsCommandList
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsCommandList_Dispose")]
		private static extern void GraphicsCommandList_Dispose(GraphicsCommandList* _this);
	}
}