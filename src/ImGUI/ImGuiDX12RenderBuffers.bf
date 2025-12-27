using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public struct ImGuiDX12RenderBuffers
	{
		public GraphicsResource pIndexBuffer;
		public GraphicsResource pVertexBuffer;
		public int32 IndexBufferSize;
		public int32 VertexBufferSize;
	}
}