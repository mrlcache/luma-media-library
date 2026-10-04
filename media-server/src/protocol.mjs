import { XMLParser, XMLValidator } from 'fast-xml-parser';

export const CD = 'urn:schemas-upnp-org:service:ContentDirectory:1';
export const CM = 'urn:schemas-upnp-org:service:ConnectionManager:1';
export const DEVICE = 'urn:schemas-upnp-org:device:MediaServer:1';
export const xml = value => String(value ?? '').replace(/[&<>"']/g, char => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[char]));
const declaration = '<?xml version="1.0" encoding="utf-8"?>';
export const envelope = body => `${declaration}<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/"><s:Body>${body}</s:Body></s:Envelope>`;
export const response = (service, action, values) => envelope(`<u:${action}Response xmlns:u="${service}">${Object.entries(values).map(([k,v]) => `<${k}>${xml(v)}</${k}>`).join('')}</u:${action}Response>`);
export const fault = (code, description) => envelope(`<s:Fault><faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring><detail><UPnPError xmlns="urn:schemas-upnp-org:control-1-0"><errorCode>${code}</errorCode><errorDescription>${xml(description)}</errorDescription></UPnPError></detail></s:Fault>`);
export function parseAction(body, header, service) {
  if (/<!DOCTYPE|<!ENTITY/i.test(body) || XMLValidator.validate(body) !== true) throw new Error('Invalid XML');
  const action = String(header || '').replace(/^"|"$/g,'').split('#');
  if (action.length !== 2 || action[0] !== service || !/^[A-Za-z]+$/.test(action[1])) throw new Error('Invalid SOAP action');
  const parsed = new XMLParser({removeNSPrefix:true,parseTagValue:false,ignoreAttributes:true,processEntities:true}).parse(body);
  const payload = parsed.Envelope?.Body?.[action[1]];
  if (payload === undefined || Array.isArray(payload)) throw new Error('Missing SOAP action');
  return {name:action[1], args:typeof payload === 'object' ? payload : {}};
}

export function didl(items, baseURL, compatibility = false) {
  return `<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:dlna="urn:schemas-dlna-org:metadata-1-0/">${items.map(item => {
    const art = item.poster ? `<upnp:albumArtURI>${xml(item.poster)}</upnp:albumArtURI>` : '';
    if (item.container) return `<container id="${xml(item.id)}" parentID="${xml(item.parentID)}" restricted="1" childCount="${item.childCount}"><dc:title>${xml(item.title)}</dc:title><upnp:class>object.container.storageFolder</upnp:class>${art}</container>`;
    const direct = `<res size="${item.size}" protocolInfo="http-get:*:${item.mime}:DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000">${baseURL}/media/${xml(item.id)}/original</res>`;
    // Original remains the default. Compatibility is an explicit alternate resource.
    const alternate = compatibility ? `<res protocolInfo="http-get:*:video/mp2t:DLNA.ORG_OP=00;DLNA.ORG_CI=1;DLNA.ORG_FLAGS=01700000000000000000000000000000">${baseURL}/media/${xml(item.id)}/compatible</res>` : '';
    return `<item id="${xml(item.id)}" parentID="${xml(item.parentID)}" restricted="1"><dc:title>${xml(item.displayTitle)}</dc:title><upnp:class>object.item.videoItem${item.kind === 'movie' ? '.movie' : ''}</upnp:class>${item.year ? `<dc:date>${item.year}-01-01</dc:date>` : ''}${item.overview ? `<dc:description>${xml(item.overview)}</dc:description>` : ''}${art}${direct}${alternate}</item>`;
  }).join('')}</DIDL-Lite>`;
}

export function description(uuid, name, baseURL) {
  return `${declaration}<root xmlns="urn:schemas-upnp-org:device-1-0"><specVersion><major>1</major><minor>0</minor></specVersion><URLBase>${baseURL}/</URLBase><device><deviceType>${DEVICE}</deviceType><friendlyName>${xml(name)}</friendlyName><manufacturer>Luma</manufacturer><modelName>Luma Media Server</modelName><modelNumber>0.1.0</modelNumber><UDN>uuid:${uuid}</UDN><serviceList>${[['ContentDirectory',CD],['ConnectionManager',CM]].map(([name,type]) => `<service><serviceType>${type}</serviceType><serviceId>urn:upnp-org:serviceId:${name}</serviceId><SCPDURL>/upnp/${name}/scpd.xml</SCPDURL><controlURL>/upnp/${name}/control</controlURL><eventSubURL>/upnp/${name}/event</eventSubURL></service>`).join('')}</serviceList></device></root>`;
}

const services = {
  ContentDirectory: {
    actions: {
      GetSearchCapabilities:[['SearchCaps','out','SearchCapabilities']], GetSortCapabilities:[['SortCaps','out','SortCapabilities']], GetSystemUpdateID:[['Id','out','SystemUpdateID']],
      Browse:[['ObjectID','in','ObjectID'],['BrowseFlag','in','BrowseFlag'],['Filter','in','Filter'],['StartingIndex','in','Index'],['RequestedCount','in','Count'],['SortCriteria','in','SortCriteria'],['Result','out','Result'],['NumberReturned','out','Count'],['TotalMatches','out','Count'],['UpdateID','out','SystemUpdateID']]
    },
    variables: {SearchCapabilities:['string',false],SortCapabilities:['string',false],SystemUpdateID:['ui4',true],ObjectID:['string',false],BrowseFlag:['string',false,['BrowseMetadata','BrowseDirectChildren']],Filter:['string',false],Index:['ui4',false],Count:['ui4',false],SortCriteria:['string',false],Result:['string',false]}
  },
  ConnectionManager: {
    actions:{ GetProtocolInfo:[['Source','out','SourceProtocolInfo'],['Sink','out','SinkProtocolInfo']],GetCurrentConnectionIDs:[['ConnectionIDs','out','CurrentConnectionIDs']],GetCurrentConnectionInfo:[['ConnectionID','in','ConnectionID'],['RcsID','out','RcsID'],['AVTransportID','out','AVTransportID'],['ProtocolInfo','out','ProtocolInfo'],['PeerConnectionManager','out','ConnectionManager'],['PeerConnectionID','out','ConnectionID'],['Direction','out','Direction'],['Status','out','ConnectionStatus']] },
    variables:{ SourceProtocolInfo:['string',true],SinkProtocolInfo:['string',true],CurrentConnectionIDs:['string',true],ConnectionID:['i4',false],RcsID:['i4',false],AVTransportID:['i4',false],ProtocolInfo:['string',false],ConnectionManager:['string',false],Direction:['string',false,['Input','Output']],ConnectionStatus:['string',false,['OK','ContentFormatMismatch','InsufficientBandwidth','UnreliableChannel','Unknown']] }
  }
};
const variableName = (key, event) => event || key.endsWith('Capabilities') ? key : `A_ARG_TYPE_${key}`;
export function scpd(service) {
  const spec = services[service];
  return `${declaration}<scpd xmlns="urn:schemas-upnp-org:service-1-0"><specVersion><major>1</major><minor>0</minor></specVersion><actionList>${Object.entries(spec.actions).map(([action,args]) => `<action><name>${action}</name><argumentList>${args.map(([name,direction,state]) => `<argument><name>${name}</name><direction>${direction}</direction><relatedStateVariable>${variableName(state,spec.variables[state][1])}</relatedStateVariable></argument>`).join('')}</argumentList></action>`).join('')}</actionList><serviceStateTable>${Object.entries(spec.variables).map(([key,[type,event,allowed]]) => `<stateVariable sendEvents="${event ? 'yes' : 'no'}"><name>${variableName(key,event)}</name><dataType>${type}</dataType>${allowed ? `<allowedValueList>${allowed.map(v=>`<allowedValue>${v}</allowedValue>`).join('')}</allowedValueList>` : ''}</stateVariable>`).join('')}</serviceStateTable></scpd>`;
}

export function range(header, size) {
  if (!header) return {start:0,end:size-1,partial:false};
  const match = /^bytes=(\d*)-(\d*)$/.exec(header);
  if (!match || (!match[1] && !match[2]) || size === 0) return null;
  let start, end;
  if (!match[1]) { const suffix = Number(match[2]); if (!Number.isSafeInteger(suffix) || suffix <= 0) return null; start = Math.max(0,size-suffix); end = size-1; }
  else { start = Number(match[1]); end = match[2] ? Math.min(Number(match[2]),size-1) : size-1; }
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start >= size || end < start) return null;
  return {start,end,partial:true};
}
