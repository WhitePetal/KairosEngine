using System;

namespace KairosEngine.Graphics
{
	[CRepr]
	public struct RootParameter
	{
		public RootParameterType ParameterType;
		public struct UnioData
		{
			public RootDescriptorTable DescriptorTable;
			public RootConstants Constants;
			public RootDescriptor Descriptor;
		}
		public UnioData Unio;
		public ShaderVisibility ShaderVisibility;
	}
}