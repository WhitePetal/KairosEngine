using System;

namespace KairosEngine.Graphics
{
	extension FenceEvent
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("FenceEvent_Create")]
		private static extern int32 FenceEvent_Create(void** p_this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("FenceEvent_Close")]
		private static extern void FenceEvent_Close(void* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("FenceEvent_Wait")]
		private static extern uint32 FenceEvent_Wait(void* _this, uint32 dwMilliseconds);
	}
}