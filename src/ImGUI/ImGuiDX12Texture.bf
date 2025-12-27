using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12Texture
	{
		public GraphicsResource* pTextureResource;
		public GraphicsCPUDescriptorHandle FontSrvCpuDescHandle;
		public GraphicsGPUDescriptorHandle FontSrvGpuDescHandle;
	}
}