using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFenceEvent
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFenceEvent_Create")]
		private static extern int32 GraphicsFenceEvent_Create(GraphicsFenceEvent* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFenceEvent_Dispose")]
		private static extern void GraphicsFenceEvent_Dispose(GraphicsFenceEvent* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFenceEvent_Wait")]
		private static extern uint32 GraphicsFenceEvent_Wait(GraphicsFenceEvent* _this, uint32 dwMilliseconds);
	}
}