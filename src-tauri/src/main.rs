// Release 构建下不弹 Windows 控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    paperflow_app_lib::run()
}
