"""Opt-in Docker/OpenSSL-only synthetic OpenVPN integration proof; no organization access."""
import subprocess,tempfile,pathlib,uuid,time,json,sys,os
ROOT=pathlib.Path(__file__).resolve().parents[3]
run='sm-vpn-positive-'+uuid.uuid4().hex[:12]
def docker(*args,check=True,timeout=30):
 r=subprocess.run(['docker',*args],capture_output=True,text=True,timeout=timeout)
 if check and r.returncode: raise RuntimeError('Docker step failed: '+args[0]+' '+r.stderr[:160])
 return r
def openssl(*args): subprocess.run(['openssl',*args],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
with tempfile.TemporaryDirectory(prefix='.sm-vpn-fixture-',dir=ROOT) as tmp:
 p=pathlib.Path(tmp);net=run+'-net';server=run+'-server';gateways=[run+'-a',run+'-b'];workers=[run+'-a1',run+'-a2',run+'-b1']
 openssl('req','-x509','-newkey','rsa:2048','-nodes','-keyout',str(p/'ca.key'),'-out',str(p/'ca.crt'),'-days','1','-subj','/CN=SyntheticVPNCA')
 for role in ['server','client']:
  openssl('req','-new','-newkey','rsa:2048','-nodes','-keyout',str(p/(role+'.key')),'-out',str(p/(role+'.csr')),'-subj','/CN='+role)
  (p/(role+'.ext')).write_text('basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\nextendedKeyUsage='+('serverAuth' if role=='server' else 'clientAuth')+'\n')
  openssl('x509','-req','-in',str(p/(role+'.csr')),'-CA',str(p/'ca.crt'),'-CAkey',str(p/'ca.key'),'-CAcreateserial','-out',str(p/(role+'.crt')),'-days','1','-extfile',str(p/(role+'.ext')))
 (p/'server.ovpn').write_text('port 1194\nproto udp\ndev tun\ntopology subnet\nserver 10.66.0.0 255.255.255.0\nca /fixture/ca.crt\ncert /fixture/server.crt\nkey /fixture/server.key\ndh none\nduplicate-cn\ndata-ciphers AES-256-GCM\nauth SHA256\nkeepalive 2 10\nverb 3\n')
 (p/'username').write_text('synthetic-user');(p/'password').write_text('synthetic-password');(p/'resolver').write_text('nameserver 127.0.0.1\noptions ndots:0\n')
 for f in p.iterdir(): f.chmod(0o600)
 try:
  docker('network','create','--label','dev.service-manager.proof='+run,net)
  server_cmd='openvpn2.6 --config /fixture/server.ovpn & while true; do printf "HTTP/1.1 200 OK\\r\\nContent-Length: 12\\r\\nConnection: close\\r\\n\\r\\nsynthetic-ok" | nc -l -p 5432; done'
  docker('run','-d','--name',server,'--network',net,'--label','dev.service-manager.proof='+run,'--cap-add=NET_ADMIN','--device=/dev/net/tun:/dev/net/tun','--mount',f'type=bind,source={p},target=/fixture,readonly','--entrypoint','/bin/sh','qmcgaw/gluetun:v3.41.0','-c',server_cmd)
  remote=json.loads(docker('inspect',server).stdout)[0]['NetworkSettings']['Networks'][net]['IPAddress']
  if len(sys.argv)>1: remote='synthetic-vpn-server'
  (p/'client.ovpn').write_text('client\ndev tun\nproto udp\nremote '+remote+' 1194\nnobind\nauth-user-pass\nremote-cert-tls server\ndata-ciphers AES-256-GCM\nauth SHA256\n<ca>\n'+(p/'ca.crt').read_text()+'</ca>\n<cert>\n'+(p/'client.crt').read_text()+'</cert>\n<key>\n'+(p/'client.key').read_text()+'</key>\n')
  (p/'client.ovpn').chmod(0o600)
  if len(sys.argv)>1:
   env=os.environ.copy();env.update(SM_VPN_FIXTURE_DIR=str(p),SM_VPN_FIXTURE_SERVER=server)
   subprocess.run([sys.argv[1],'vpn_manager_shares_gateways_and_verifies_loss_cleanup','--ignored','--exact','--nocapture'],env=env,check=True)
  else:
   for gw in gateways:
    args=['run','-d','--name',gw,'--network',net,'--label','dev.service-manager.proof='+run,'--cap-drop=ALL','--cap-add=NET_ADMIN','--cap-add=NET_RAW','--cap-add=CHOWN','--cap-add=DAC_OVERRIDE','--cap-add=SETUID','--cap-add=SETGID','--device=/dev/net/tun:/dev/net/tun','--security-opt=no-new-privileges:true','--restart=no']
    for fn,dest in [('client.ovpn','/gluetun/custom.conf'),('username','/run/secrets/vpn_username'),('password','/run/secrets/vpn_password')]: args+=['--mount',f'type=bind,source={p/fn},target={dest},readonly']
    for env in ['VPN_SERVICE_PROVIDER=custom','VPN_TYPE=openvpn','OPENVPN_CUSTOM_CONFIG=/gluetun/custom.conf','OPENVPN_USER_SECRETFILE=/run/secrets/vpn_username','OPENVPN_PASSWORD_SECRETFILE=/run/secrets/vpn_password','FIREWALL=on','DNS_ADDRESS=127.0.0.1','DNS_UPDATE_PERIOD=0','HEALTH_TARGET_ADDRESSES=10.66.0.1:5432','HEALTH_ICMP_TARGET_IPS=10.66.0.1']:args+=['--env',env]
    args+=['qmcgaw/gluetun:v3.41.0'];docker(*args)
   deadline=time.monotonic()+50
   while time.monotonic()<deadline:
    states=[json.loads(docker('inspect',gw).stdout)[0]['State'] for gw in gateways]
    if all(s.get('Health',{}).get('Status')=='healthy' for s in states):break
    time.sleep(1)
   healthy=[s.get('Health',{}).get('Status')=='healthy' for s in states]
   print(json.dumps({'bothGatewaysHealthy':all(healthy),'gatewayHealth':healthy}),flush=True)
   if not all(healthy):
    for gw in [server,*gateways]:
     logs=docker('logs',gw,check=False).stdout+docker('logs',gw,check=False).stderr
     for line in logs.splitlines():
      if any(t in line for t in ['ERROR','FATAL','Initialization Sequence Completed','Permission denied','operation not permitted','[openvpn]','Options error']): print('sanitized-init: '+line.replace(str(p),'[fixture]')[:240])
    raise RuntimeError('Synthetic gateways did not become healthy')
   for i,w in enumerate(workers):
    gw=gateways[0 if i<2 else 1]
    docker('run','-d','--name',w,'--network','container:'+gw,'--label','dev.service-manager.proof='+run,'--cap-drop=ALL','--security-opt=no-new-privileges:true','--mount',f'type=bind,source={p/"resolver"},target=/etc/resolv.conf,readonly','alpine:3.23.3','sleep','180')
   def probe(w):return docker('exec',w,'wget','-T','3','-qO-','http://10.66.0.1:5432',check=False,timeout=7).stdout.strip()=='synthetic-ok'
   before=[probe(w) for w in workers]
   print(json.dumps({'allThreeWorkersReachVpnTarget':all(before),'workerReachability':before}),flush=True)
   docker('stop','--time','1',workers[0]);print(json.dumps({'remainingSharedWorkerStillWorks':probe(workers[1]),'otherVpnStillWorks':probe(workers[2])}),flush=True)
   docker('stop','--time','1',gateways[0])
   deadline=time.monotonic()+20
   while time.monotonic()<deadline:
    state=json.loads(docker('inspect',gateways[0]).stdout)[0]['State']
    if not state['Running'] or state.get('Health',{}).get('Status')=='unhealthy':break
    time.sleep(1)
   print(json.dumps({'failedVpnHealthUnhealthy':not state['Running'] or state.get('Health',{}).get('Status')=='unhealthy','failedVpnWorkerBlocked':not probe(workers[1]),'otherVpnWorkerUnaffected':probe(workers[2])}),flush=True)

 finally:
  owner_path=p/'state/vpn/owner'
  if owner_path.exists():
   owner=owner_path.read_text().strip()
   for resource in docker('ps','-aq','--filter','label=io.service-manager.vpn.owner='+owner,check=False).stdout.splitlines():docker('rm','-f',resource,check=False)
   for network_id in docker('network','ls','-q','--filter','label=io.service-manager.vpn.owner='+owner,check=False).stdout.splitlines():
    docker('network','disconnect','-f',network_id,server,check=False);docker('network','rm',network_id,check=False)
  for name in [*workers,*gateways,server]:docker('rm','-f',name,check=False)
  docker('network','rm',net,check=False)
 print('fixtureCleanupComplete',flush=True)
