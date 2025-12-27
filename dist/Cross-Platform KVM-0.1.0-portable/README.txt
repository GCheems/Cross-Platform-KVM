Cross-Platform KVM v0.1.0
================================

使用方法：
1. 双击 "Cross-Platform KVM.exe" 启动应用
2. 首次运行需要管理员权限
3. 确保防火墙允许 UDP 5353 端口

配置防火墙（以管理员身份运行 PowerShell）：
netsh advfirewall firewall add rule name="Cross-Platform KVM" dir=in action=allow program="%CD%\Cross-Platform KVM.exe" enable=yes
netsh advfirewall firewall add rule name="Cross-Platform KVM UDP" dir=in action=allow protocol=UDP localport=5353 enable=yes

更多信息请访问项目主页。
