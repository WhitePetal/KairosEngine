#include "KairosWindow.h"

LRESULT CALLBACK ProxyWndProc(HWND hwnd, UINT msg, WPARAM wparam, LPARAM lparam)
{
	WndProcCallback pCallback;

	if (msg == WM_NCCREATE)
	{
		CREATESTRUCT* pCreateStruct = reinterpret_cast<CREATESTRUCT*>(lparam);
		pCallback = reinterpret_cast<WndProcCallback>(pCreateStruct->lpCreateParams);
		SetWindowLongPtr(hwnd, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(pCallback));
	}
	else
	{
		pCallback = reinterpret_cast<WndProcCallback>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
	}

	if (pCallback != nullptr)
	{
		return pCallback(hwnd, msg, wparam, lparam);
	}
	return DefWindowProc(hwnd, msg, wparam, lparam);
}

KAIROS_EXPORT_BEGIN

HWND KAIROS_API KairosInitializeWindow(HINSTANCE hInstance, int ShowWnd, int width, int height, bool fullScreen, LPCWSTR windowName, LPCWSTR windowTitle, WndProcCallback wndProc)
{
	HWND hwnd = {};
	if (fullScreen)
	{
		HMONITOR hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
		MONITORINFO mi = { sizeof(mi) };
		GetMonitorInfo(hmon, &mi);

		width = mi.rcMonitor.right - mi.rcMonitor.left;
		height = mi.rcMonitor.bottom - mi.rcMonitor.top;
	}

	WNDCLASSEX wc;
	wc.cbSize = sizeof(WNDCLASSEX);
	wc.style = CS_HREDRAW | CS_VREDRAW;
	wc.lpfnWndProc = ProxyWndProc;
	wc.cbClsExtra = NULL;
	wc.cbWndExtra = NULL;
	wc.hInstance = hInstance;
	wc.hIcon = LoadIcon(NULL, IDI_APPLICATION);
	wc.hCursor = LoadCursor(NULL, IDC_ARROW);
	wc.hbrBackground = (HBRUSH)(COLOR_WINDOW + 2);
	wc.lpszMenuName = NULL;
	wc.lpszClassName = windowName;
	wc.hIconSm = LoadIcon(NULL, IDI_APPLICATION);

	if (!RegisterClassEx(&wc))
	{
		MessageBox(NULL, L"Error registering class", L"ERROR", MB_OK | MB_ICONERROR);
		return hwnd;
	}

	hwnd = CreateWindowEx(
		NULL,
		windowName, windowTitle,
		WS_OVERLAPPEDWINDOW,
		CW_USEDEFAULT, CW_USEDEFAULT,
		width, height,
		NULL,
		NULL,
		hInstance,
		wndProc);

	if (!hwnd)
	{
		MessageBox(NULL, L"Error creating window", L"Error", MB_OK | MB_ICONERROR);
		return hwnd;
	}

	if (fullScreen)
	{
		SetWindowLong(hwnd, GWL_STYLE, 0);
	}

	ShowWindow(hwnd, ShowWnd);
	UpdateWindow(hwnd);
	return hwnd;
}

int KAIROS_API KairosDefWindowProcW(HWND hWnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
	return DefWindowProcW(hWnd, msg, wParam, lParam);
}

void KAIROS_API KairosDestroyWindow(HWND hWnd)
{
	DestroyWindow(hWnd);
}

void KAIROS_API KairosPostQuitMessage(int nExitCode)
{
	PostQuitMessage(nExitCode);
}

void KAIROS_API KairosMainLoop()
{
	MSG msg;
	ZeroMemory(&msg, sizeof(MSG));

	while (true)
	{
		if (PeekMessage(&msg, NULL, 0, 0, PM_REMOVE))
		{
			if (msg.message == WM_QUIT)
				break;

			TranslateMessage(&msg);
			DispatchMessage(&msg);
		}
		else
		{

		}
	}
}

void KAIROS_API KairosInitMSG(MSG* pMsg)
{
	ZeroMemory(pMsg, sizeof(MSG));
}

int KAIROS_API KairosPeekMessage(MSG* pMsg)
{
	return PeekMessage(pMsg, NULL, 0, 0, PM_REMOVE);
}

int KAIROS_API KairosTranslateMessage(MSG* pMsg)
{
	return TranslateMessage(pMsg);
}

LRESULT KAIROS_API KairosDispatchMessage(MSG* pMsg)
{
	return DispatchMessage(pMsg);
}



KAIROS_EXPORT_END