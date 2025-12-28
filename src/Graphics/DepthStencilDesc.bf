using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct DepthStencilDesc
	{
		public bool DepthEnable;
		public DepthWriteMask DepthWriteMask;
		public ComparisonFunc DepthFunc;
		public bool StencilEnable;
		public uint8 StencilReadMask;
		public uint8 StencilWriteMask;
		public DepthStencilOpDesc FrontFace;
		public DepthStencilOpDesc BackFace;
	}
}