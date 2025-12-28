using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct BlendDesc
	{
		public bool AlphaToCoverageEnable;
		public bool IndependentBlendEnable;
		public RenderTargetBlendDesc[8] RenderTarget;

		public this(bool alphaToCoverageEnable, ref RenderTargetBlendDesc renderTarget)
		{
			AlphaToCoverageEnable = alphaToCoverageEnable;
			IndependentBlendEnable = false;
			RenderTarget[0] = renderTarget;
		}
	}
}