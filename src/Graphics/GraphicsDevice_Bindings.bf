using System;

namespace KairosEngine.Graphics
{
	extension GraphicsDevice
	{
		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Create")]
		private static extern Pointer GraphicsDevice_Create();

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("GraphicsDevice_Initialize")]
		private static extern int GraphicsDevice_Initialize(Pointer _this);

		[Import("DX12.lib"), CallingConvention(.Cdecl), LinkName("Graphics_DeInitialize")]
		private static extern void Graphics_DeInitialize(Pointer _this);
	}
}