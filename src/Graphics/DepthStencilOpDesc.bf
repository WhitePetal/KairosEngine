using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct DepthStencilOpDesc
	{
		public DepthStencilOp StencilFailOp;
		public DepthStencilOp StencilDepthFailOp;
		public DepthStencilOp StencilPassOp;
		public ComparisonFunc StencilFunc;
	}
}