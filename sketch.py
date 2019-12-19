import subprocess

for i in range(9):
    subprocess.call(['cargo', 'run', '--release', '--', 
                     'bach_kyrie.wav', f'out_{i}.wav', 
                     '-f', '3', '-w', str(2 ** (15 - i))])
