using System;

namespace KairosEngine.Graphics
{
	extension GraphicsFence
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFence_Dispose")]
		private static extern void GraphicsFence_Dispose(GraphicsFence* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFence_GetCompletedValue")]
		private static extern uint64 GraphicsFence_GetCompletedValue(GraphicsFence* _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsFence_SetEventOnCompletion")]
		private static extern int32 GraphicsFence_SetEventOnCompletion(GraphicsFence* _this, uint64 fenceValue, GraphicsFenceEvent* pGraphicsFenceEvent);
	}
}