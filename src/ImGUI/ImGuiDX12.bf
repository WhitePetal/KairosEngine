using ImGui;
using KairosEngine.Graphics;

namespace KairosEngine.ImGUI
{
	public static class ImGuiDX12
	{
		public static ImGuiDX12Data* GetBackendData()
		{
			return ImGui.GetCurrentContext() != null ? (ImGuiDX12Data*)ImGui.GetIO().BackendRendererUserData : null;
		}

		public static void InvalidateDeviceObjects()
		{
			ImGuiDX12Data* bd = GetBackendData();
			if(bd == null || bd.pDevice == null)
				return;

			bd.pFactory.Dispose();
			bd.pCommandQueue.Dispose();
			bd.CommandQueueOwned = false;
			bd.pRootSignature.Dispose();
			bd.pPipelineState.Dispose();
			if(bd.pTexUploadBufferMapped != null)
			{
				bd.pTexUploadBuffer.Unmap(0, 0, bd.pTexUploadBufferSize);
				bd.pTexUploadBufferMapped = null;
			}
			bd.pTexUploadBuffer.Dispose();
			bd.pTexCmdList.Dispose();
			bd.pTexCmdAllocator.Dispose();
			bd.pFence.Dispose();
			bd.FenceEvent.Dispose();
			bd.FenceEvent = default;

			// destroy all textures
			var textures = ImGui.GetPlatformIO().Textures;
			for(int32 i = 0; i < textures.Size; ++i)
			{
				ImGui.TextureData* tex = textures.Data[i];
				if(tex.RefCount == 1)
					DestroyTexture(tex);
			}

			for(uint32 i = 0; i < bd.NumFramesInFlight; ++i)
			{
				ImGuiDX12RenderBuffers* fr = &bd.pFrameResources[i];
				fr.pIndexBuffer.Dispose();
				fr.pVertexBuffer.Dispose();
			}
		}

		public static void DestroyTexture(ImGui.TextureData* tex)
		{
			ImGuiDX12Texture* backend_tex = (ImGuiDX12Texture*)tex.BackendUserData;
			if(backend_tex != null)
			{
				ImGuiDX12Data* bd = GetBackendData();
				bd.InitInfo.SrvDescriptorFreeFn(&bd.InitInfo, backend_tex.FontSrvCpuDescHandle, backend_tex.FontSrvGpuDescHandle);
				backend_tex.pTextureResource.Dispose();
				backend_tex.FontSrvCpuDescHandle.Ptr = 0;
				backend_tex.FontSrvGpuDescHandle.Ptr = 0;
				ImGui.MemFree(backend_tex);

				tex.SetTexID(0);
				tex.BackendUserData = null;
			}
			tex.SetStatus(ImGui.TextureStatus.Destroyed);
		}

		public static int32 CreateDeviceObjects()
		{
			ImGuiDX12Data* bd = GetBackendData();
			if(bd == null || bd.pDevice == null)
				return 1;
			if(bd.pPipelineState != null)
				InvalidateDeviceObjects();

			GraphicsFactory* pFactory = new GraphicsFactory();
			int32 hr = pFactory.Create();
			if(hr > 0)
				return hr;

			bool allow_tearing = false;
			bd.pFactory = pFactory;
			bd.pFactory.CheckFeatureSupport(GraphicsFeature.PRESENT_ALLOW_TEARING, &allow_tearing, sizeof(bool));
			bd.TearingSupport = allow_tearing;

			// Create the root signature
			GraphicsDescriptorRange descRange = GraphicsDescriptorRange
			{
				RangeType = DescriptorRangeType.SRV,
				NumDescriptors = 1,
				BaseShaderRegister = 0,
				RegisterSpace = 0,
				OffsetInDescriptorsFromTableStart = 0
			};

			GraphicsRootParameter[] param = scope GraphicsRootParameter[2](?);

			param[0].ParameterType = RootParameterType._32BIT_CONSTANTS;
			param[0].Unio.Constants.ShaderRegister = 0;
			param[0].Unio.Constants.RegisterSpace = 0;
			param[0].Unio.Constants.Num32BitValues = 16;
			param[0].ShaderVisibility = ShaderVisibility.VERTEX;

			param[1].ParameterType = RootParameterType.DESCRIPTOR_TABLE;
			param[1].Unio.DescriptorTable.NumDescriptorRanges = 1;
			param[1].Unio.DescriptorTable.pDescriptorRanges = &descRange;
			param[1].ShaderVisibility = ShaderVisibility.FRAGMENT;

			GraphicsStaticSamplerDesc[] staticSampler = scope GraphicsStaticSamplerDesc[1](?);
			staticSampler[0].Filter = SamplerFilter.MIN_MAG_MIP_LINEAR;
			staticSampler[0].AddressU = TextureAddressMode.CLAMP;
			staticSampler[0].AddressV = TextureAddressMode.CLAMP;
			staticSampler[0].AddressW = TextureAddressMode.CLAMP;
			staticSampler[0].MipLODBias = 0.0f;
			staticSampler[0].MaxAnisotropy = 0;
			staticSampler[0].ComparisonFunc = ComparisonFunc.ALWAYS;
			staticSampler[0].BorderColor = StaticBorderColor.TRANSPARENT_BLACK;
			staticSampler[0].MinLOD = 0.0f;
			staticSampler[0].MaxLOD = float.MaxValue;
			staticSampler[0].ShaderRegister = 0;
			staticSampler[0].RegisterSpace = 0;
			staticSampler[0].ShaderVisibility = ShaderVisibility.FRAGMENT;

			GraphicsRootSignatureDesc desc = GraphicsRootSignatureDesc
			{
				NumParameters = uint32(param.Count),
				pParameters = param.Ptr,
				NumStaticSamplers = 1,
				pStaticSamplers = staticSampler.Ptr,
				Flags =
					RootSignatureFlags.ALLOW_INPUT_ASSEMBLER_INPUT_LAYOUT |
					RootSignatureFlags.DENY_HULL_SHADER_ROOT_ACCESS |
					RootSignatureFlags.DENY_DOMAIN_SHADER_ROOT_ACCESS |
					RootSignatureFlags.DENY_GEOMETRY_SHADER_ROOT_ACCESS
			};

			(hr, )bd.pDevice.CreateRootSignature(ref desc);
		}

		public static int32 NewFrame()
		{
			TestA ta = new TestA();
			TestA[] ar = new TestA[10](?);
			ar[0] = ta;
			ar[1] = TestA();
			ImGuiDX12Data* bd = GetBackendData();
			if(bd == null)
				return int32(ErrorCodes.BackendDataIsNull);
			if(bd.pPipelineState == null)
				return CreateDeviceObjects();

			return int32(ErrorCodes.ImGUI_Success);
		}

		public static void InitMultiViewportSupport()
		{

		}
	}
}