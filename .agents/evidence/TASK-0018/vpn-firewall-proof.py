import subprocess,tempfile,pathlib,uuid,time,json,shutil
run='sm-vpn-proof-'+uuid.uuid4().hex[:12]
def docker(*args,check=True,timeout=30):
 r=subprocess.run(['docker',*args],capture_output=True,text=True,timeout=timeout)
 if check and r.returncode: raise RuntimeError('Docker step failed: '+args[0]+' '+r.stderr[:200])
 return r
with tempfile.TemporaryDirectory(prefix='.sm-vpn-fixture-',dir=str(pathlib.Path(__file__).resolve().parents[3])) as tmp:
 p=pathlib.Path(tmp)
 subprocess.run(['openssl','req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(p/'key'),'-out',str(p/'ca'),'-days','1','-subj','/CN=synthetic-vpn-proof'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
 (p/'client.ovpn').write_text('client\ndev tun\nproto udp\nremote 198.51.100.1 1194\nnobind\nauth-user-pass\nremote-cert-tls server\n<ca>\n'+(p/'ca').read_text()+'</ca>\n')
 (p/'username').write_text('synthetic-user');(p/'password').write_text('synthetic-password');(p/'resolver').write_text('nameserver 127.0.0.1\noptions ndots:0\n')
 for f in p.iterdir(): f.chmod(0o600)
 net=run+'-net';gw=run+'-gateway';worker=run+'-worker';target=run+'-target'
 try:
  docker('network','create','--label','dev.service-manager.proof='+run,net)
  docker('run','-d','--name',target,'--network',net,'--label','dev.service-manager.proof='+run,'alpine:3.23.3','sh','-c','while true; do printf \"HTTP/1.1 200 OK\\r\\nContent-Length: 12\\r\\nConnection: close\\r\\n\\r\\nsynthetic-ok\" | nc -l -p 8080; done')
  target_ip=json.loads(docker('inspect',target).stdout)[0]['NetworkSettings']['Networks'][net]['IPAddress']
  baseline=docker('run','--rm','--network',net,'--label','dev.service-manager.proof='+run,'alpine:3.23.3','wget','-T','3','-qO-',f'http://{target_ip}:8080').stdout.strip()
  print(json.dumps({'baselineReachable':baseline=='synthetic-ok'}))
  args=['run','-d','--name',gw,'--network',net,'--label','dev.service-manager.proof='+run,'--cap-drop=ALL','--cap-add=NET_ADMIN','--cap-add=CHOWN','--cap-add=DAC_OVERRIDE','--device=/dev/net/tun:/dev/net/tun','--security-opt=no-new-privileges:true','--restart=no']
  for fn,dest in [('client.ovpn','/gluetun/custom.conf'),('username','/run/secrets/vpn_username'),('password','/run/secrets/vpn_password')]: args+=['--mount',f'type=bind,source={p/fn},target={dest},readonly']
  for env in ['VPN_SERVICE_PROVIDER=custom','VPN_TYPE=openvpn','OPENVPN_CUSTOM_CONFIG=/gluetun/custom.conf','OPENVPN_USER_SECRETFILE=/run/secrets/vpn_username','OPENVPN_PASSWORD_SECRETFILE=/run/secrets/vpn_password','FIREWALL=on','DNS_ADDRESS=127.0.0.1','HEALTH_TARGET_ADDRESSES=cloudflare.com:443'] :args+=['--env',env]
  args+=['qmcgaw/gluetun:v3.41.0'];docker(*args)
  time.sleep(5)
  state=json.loads(docker('inspect',gw).stdout)[0]['State']
  logs=docker('logs',gw,check=False).stdout+docker('logs',gw,check=False).stderr
  print(json.dumps({'gatewayRunning':state['Running'],'exitCode':state['ExitCode'],'firewallEnabled': 'Firewall enabled' in logs or 'firewall enabled' in logs,'operationNotPermitted':'operation not permitted' in logs,'permissionDenied':'permission denied' in logs,'fatal':any(x in logs for x in ['ERROR','FATAL'])}))
  for line in logs.splitlines():
   if any(x in line.lower() for x in ['error','fatal','permission','capabil','firewall enabled']):
    print('sanitized-init: '+line.replace(str(p),'[fixture]')[:300])
  if state['Running']:
   wid=docker('run','-d','--name',worker,'--network','container:'+gw,'--label','dev.service-manager.proof='+run,'--cap-drop=ALL','--security-opt=no-new-privileges:true','--mount',f'type=bind,source={p/"resolver"},target=/etc/resolv.conf,readonly','alpine:3.23.3','sleep','120').stdout.strip()
   resolver=docker('exec',worker,'cat','/etc/resolv.conf').stdout
   rw=docker('exec',worker,'sh','-c','echo nameserver 8.8.8.8 > /etc/resolv.conf',check=False)
   egress=docker('exec',worker,'sh','-c','wget -T 3 -qO- http://1.1.1.1 >/dev/null 2>&1',check=False,timeout=8)
   local_egress=docker('exec',worker,'sh','-c',f'wget -T 3 -qO- http://{target_ip}:8080 >/dev/null 2>&1',check=False,timeout=8)
   dns=docker('exec',worker,'sh','-c','timeout 4 nslookup example.com >/dev/null 2>&1',check=False,timeout=8)
   rules=docker('exec',gw,'iptables','-S',check=False).stdout
   print(json.dumps({'fixedResolver':resolver=='nameserver 127.0.0.1\noptions ndots:0\n','resolverWriteBlocked':rw.returncode!=0,'noTunnelEgressBlocked':egress.returncode!=0,'noTunnelDnsBlocked':dns.returncode!=0,'knownReachableNonTunnelTargetBlocked':local_egress.returncode!=0,'outputPolicyDrop':'-P OUTPUT DROP' in rules,'inputPolicyDrop':'-P INPUT DROP' in rules}))
 finally:
  for name in [worker,gw,target]: docker('rm','-f',name,check=False)
  docker('network','rm',net,check=False)
 print('fixtureCleanupComplete')
