using System;

namespace KairosEngine.Graphics
{
 	extension GraphicsPipelineState
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsPipelineState_Dispose")]
		private static extern void GraphicsPipelineState_Dispose(GraphicsPipelineState* _this);
	}
}