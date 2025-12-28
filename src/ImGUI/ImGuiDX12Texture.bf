using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12Texture
	{
		public GraphicsResource TextureResource;
		public DescriptorCpuHandle FontSrvCpuDescHandle;
		public DescriptorGpuHandle FontSrvGpuDescHandle;
	}
}