using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12RenderBuffers
	{
		public GraphicsResource IndexBuffer;
		public GraphicsResource VertexBuffer;
		public int32 IndexBufferSize;
		public int32 VertexBufferSize;
	}
}