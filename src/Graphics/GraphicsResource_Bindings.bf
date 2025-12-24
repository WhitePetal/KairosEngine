using System;

namespace KairosEngine.Graphics
{
	extension GraphicsResource
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsResource_Dispose")]
		private static extern void GraphicsResource_Dispose(GraphicsResource* _this);
	}
}