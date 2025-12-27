using System;

namespace KairosEngine.Graphics
{
	public class GraphicsPipelineState
	{
		public void* pInternalPipelineState;

		public ~this()
		{
			if(pInternalPipelineState != null)
			{
				GraphicsPipelineState_Dispose(pInternalPipelineState);
				pInternalPipelineState = null;
			}
		}
	}
}