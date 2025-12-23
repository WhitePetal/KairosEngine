using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFenceEvent
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFenceEvent_Create")]
		private static extern CreateResult GraphicsFenceEvent_Create();

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFenceEvent_Dispose")]
		private static extern void GraphicsFenceEvent_Dispose(void* _this);
	}
}