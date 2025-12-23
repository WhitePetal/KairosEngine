using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFence
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFence_Dispose")]
		private static extern void GraphicsFence_Dispose(void* _this);
	}
}