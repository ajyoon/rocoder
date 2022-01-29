from datetime import datetime
import subprocess

hour = datetime.now().hour

if hour < 8 or hour > 20:
    subprocess.run(['systemctl', 'stop', 'installation'])
else:
    subprocess.run(['systemctl', 'start', 'installation', '--now'])
