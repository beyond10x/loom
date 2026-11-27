import {test, expect} from '@playwright/test'
import {readFileSync} from 'node:fs'
import {parse} from 'yaml'

function file(path) { return readFileSync(new URL(path, import.meta.url), 'utf8') }
function rows(name) { return JSON.parse(file('../../fixtures/' + name + '.json')).rows }
const html = file('../../dist/index.html')
const fixture = {...rows('snapshot')[0], agents:rows('agents'), runs:rows('runs'), stats:rows('statistics')[0]}
const scenarios = parse(file('../../tests/overview.yaml')).tests
const address = (page, at) => page.locator('[data-address="' + at + '"]')

async function serve(page, responder, specResponder, dispatcher) {
  await page.route('**/*', async route => {
    const url = new URL(route.request().url())
    expect(url.origin).toBe('http://dashboard.test')
    if (url.pathname === '/') return route.fulfill({body:html, contentType:'text/html'})
    if (url.pathname==='/api/launch-templates' || url.pathname==='/api/launch' || url.pathname.endsWith('/dispatch')) {
      expect(url.search).toBe('')
      expect(route.request().headers().authorization).toBe('Bearer test-viewer')
      return route.fulfill(await dispatcher(url,route.request()))
    }
    if (url.pathname.startsWith('/api/agents/')) {
      expect(url.search).toBe('')
      expect(route.request().method()).toBe('GET')
      expect(route.request().headers().authorization).toBe('Bearer test-viewer')
      if (url.pathname.endsWith('/routing') && route.request().headers().accept === 'application/json') {
        return route.fulfill({json:{id:'scout',source:'launch_catalog',agent:{selection:'small',upstream_model:'example/Agent',provider_id:'local',base_origin:'http://127.0.0.1:8080/v1',protocol:'chat_completions'},classifier:{selection:'small',upstream_model:'example/Classifier',provider_id:'local',protocol:'chat_completions'}}})
      }
      expect(route.request().headers().accept).toBe('application/yaml')
      return route.fulfill(await specResponder(url, route.request()))
    }
    expect(url.pathname).toBe('/api/snapshot')
    expect(route.request().method()).toBe('GET')
    if (route.request().headers().authorization !== 'Bearer test-viewer') return route.fulfill({status:401})
    const override = responder?.(url, route.request())
    if (override) return route.fulfill(override)
    const agent = url.searchParams.get('agent')
    const agents = fixture.agents.filter(row => !agent || row.id === agent)
    const runs = fixture.runs.filter(row => !agent || row.agent_id === agent)
    return route.fulfill({json:{...fixture,agents,runs},headers:{ETag:'"demo"'}})
  })
  await page.goto('http://dashboard.test/')
  await expect(page.getByRole('link', {name:'API design'})).toHaveAttribute('href','/api/openapi.json')
  await page.getByLabel('Viewer token').fill('test-viewer')
  await page.getByRole('button',{name:'Connect'}).click()
}
for (const scenario of scenarios) {
  test(scenario.name, async ({page}) => {
    await serve(page, null, null, () => ({json:{templates:rows('templates')}}))
    for (const step of scenario.steps) {
      if (step.open === 'dispatch') await page.getByRole('link',{name:'Dispatch',exact:true}).click()
      if (step.open === 'overview') await expect(page.getByRole('heading',{name:'Server overview'})).toBeVisible()
      if (step.select) await page.getByRole('radio',{name:step.select.endsWith('/start') ? 'Start agent' : 'Send task'}).check()
      if (step.expect?.not_text) await expect(address(page,step.expect.at)).not.toContainText(step.expect.not_text)
      if (step.act) await address(page,step.act).click()
      if (step.expect?.text) await expect(address(page,step.expect.at)).toContainText(step.expect.text)
      if (step.expect?.rows !== undefined) {
        const {at,rows} = step.expect
        const rowItems = address(page,at).locator('[data-address^="' + at + '/rows/"]')
        const records = rowItems.and(page.locator('article,tr'))
        if (typeof rows === 'number') await expect(records).toHaveCount(rows)
        else await expect.poll(() => records.evaluateAll(nodes => nodes.map(n=>n.dataset.address))).toEqual(rows.map(id=>at+'/rows/'+id))
      }
    }
    await page.reload()
    await expect(address(page,scenario.steps.at(-1).expect.at)).toContainText(scenario.steps.at(-1).expect.text)
  })
}

test('polls conditionally and clears previously seen rows when permission is revoked', async ({page}) => {
  let status=200, conditional=false
  await serve(page,(url,request) => {
    if (request.headers()['if-none-match']) conditional=true
    return status===200 ? null : {status}
  })
  await expect(page.getByText('inspect files')).toBeVisible()
  await expect(page.getByText('Partial data')).toBeVisible()
  await expect(page.getByText('Unavailable',{exact:true}).first()).toBeVisible()
  status=304
  await page.getByRole('button',{name:'Refresh'}).click()
  await expect.poll(()=>conditional).toBe(true)
  status=503
  await page.getByRole('button',{name:'Refresh'}).click()
  await expect(page.getByRole('status')).toContainText('Connection lost')
  await expect(page.getByText('inspect files')).toBeVisible()
  status=403
  await page.getByRole('button',{name:'Retry'}).click()
  await expect(page.getByRole('heading',{name:'Access denied'})).toBeVisible()
  await expect(page.getByText('inspect files')).toHaveCount(0)
  expect(await page.evaluate(()=>sessionStorage.length)).toBe(0)
})

test('UTC interval and empty inventory', async ({page}) => {
  let range
  await serve(page,url=>{
    if (!url.searchParams.has('from')) return
    range = Object.fromEntries(url.searchParams)
    return {json:{...fixture,runs:[],agents:[],stats:{...fixture.stats,active:0,total:0,completed:0}}}
  })
  await page.getByLabel('From (UTC)').fill('2026-11-22T12:00:00')
  await page.getByLabel('To (UTC, exclusive)').fill('2026-11-22T11:00:00')
  await page.getByRole('button',{name:'Apply'}).click()
  await expect(page.getByRole('alert')).toContainText('End must be after start')
  await page.getByLabel('To (UTC, exclusive)').fill('2026-11-23T12:00:00')
  await page.getByRole('button',{name:'Apply'}).click()
  await expect(page.getByText('No runs in this interval.')).toBeVisible()
  await expect(page.getByText('No visible agents.')).toBeVisible()
  expect(range).toEqual({from:'2026-11-22T12:00:00.000Z',to:'2026-11-23T12:00:00.000Z'})
})

test('shows observed metering and drills into terminal timestamps, refusals and budgets', async ({page}) => {
  const oldRun = {...fixture.runs[0], id:'old', ended_at:null, started_at:null, outcome:'stopped', stop_reason:'budget exhausted', steps:[{action:'write',outcome:'refused',steps_left:0,model_calls_left:null}]}
  const stats = {...fixture.stats,total:2,failed:1,refused_steps:1,budget_stops:1,cost:0.000014,currency:'USD',coverage:{model_calls:1,input_tokens:1,output_tokens:1,cost:1,total:2}}
  await serve(page,()=>({json:{...fixture, runs:[...fixture.runs,oldRun],stats}}))
  await expect(page.getByText('USD 0.000014')).toBeVisible()
  await expect(page.getByText('Measured in 1 / 2 turns').first()).toBeVisible()
  await page.getByText('Completions by day (UTC)').click()
  await page.getByRole('button',{name:'2026-11-20'}).click()
  await expect(page.getByRole('combobox')).toHaveValue('day:2026-11-20')
  await expect(page.locator('tbody tr')).toHaveCount(1)
  await page.getByRole('link',{name:'prior-inspection'}).click()
  await expect(page.locator('ol').first().locator('li strong')).toHaveText(['accepted','effect_invoked','terminal'])
  await page.goBack()
  await page.getByRole('combobox').selectOption('refused')
  await expect(page.locator('tbody tr')).toHaveCount(1)
  await expect(page.locator('tbody tr')).toContainText('stopped')
  await page.getByRole('combobox').selectOption('budget')
  await expect(page.locator('tbody tr')).toHaveCount(1)
})

test('missing or unauthorized agent deep link clears visible rows', async ({page}) => {
  await serve(page,url=>url.searchParams.has('agent') ? {status:404} : null)
  await address(page,'pages/overview/sections/agents/rows/scout/row_actions/open').click()
  await expect(page.getByRole('status')).toContainText('unavailable to this viewer')
  await expect(page.getByText('inspect files')).toHaveCount(0)
  await page.getByRole('link',{name:'Return to server overview'}).click()
  await expect(page.getByText('inspect files')).toBeVisible()
})

test('AgentSpec loads only on request, preserves exact YAML, and clears on navigation and logout', async ({page}) => {
  const spec = rows('agent-spec')[0]
  const requests = []
  let release
  const gate = new Promise(resolve => { release = resolve })
  await serve(page, null, async url => {
    requests.push(url.pathname)
    await gate
    return {contentType:'application/yaml; charset=utf-8', body:spec.source, headers:{ETag:'"' + spec.spec_revision + '"'}}
  })
  await expect(page.getByRole('button',{name:'View AgentSpec'})).toHaveCount(0)
  await address(page,'pages/overview/sections/agents/rows/scout/row_actions/open').click()
  const view = page.getByRole('button',{name:'View AgentSpec'})
  await expect(view).toBeVisible()
  expect(requests).toEqual([])
  await view.click()
  await expect(page.getByRole('status')).toHaveText('Loading AgentSpec…')
  release()
  const panel = page.getByRole('region',{name:'AgentSpec inspector'})
  await expect(panel.getByRole('heading',{name:'Scout',exact:true})).toBeVisible()
  await expect(page.getByRole('tab',{name:'Profile'})).toHaveAttribute('aria-selected','true')
  await expect(panel.locator('pre')).toHaveCount(0)
  await expect(page).toHaveURL(/tab=spec/)
  await page.getByRole('tab',{name:'Source YAML'}).click()
  await expect(panel).toContainText(spec.spec_revision)
  expect(await panel.locator('pre').textContent()).toBe(spec.source)
  expect(await panel.locator('markup').count()).toBe(0)
  expect(requests).toEqual(['/api/agents/scout/spec'])
  await page.getByRole('button',{name:'Hide AgentSpec'}).click()
  await expect(panel).toHaveCount(0)
  await view.click()
  await expect(panel.getByRole('tab',{name:'Profile'})).toHaveAttribute('aria-selected','true')
  await page.getByRole('link',{name:'Server overview'}).click()
  await address(page,'pages/overview/sections/agents/rows/reviewer/row_actions/open').click()
  await expect(view).toBeVisible()
  await expect(panel).toHaveCount(0)
  await page.goBack()
  await address(page,'pages/overview/sections/agents/rows/scout/row_actions/open').click()
  await view.click()
  await expect(panel.getByRole('heading',{name:'Scout',exact:true})).toBeVisible()
  expect(await page.evaluate(()=>JSON.stringify([sessionStorage,localStorage]))).not.toContain('Display <markup>')
  await page.getByRole('button',{name:'Sign out'}).click()
  await expect(page.getByRole('heading',{name:'Viewer access'})).toBeVisible()
  await expect(panel).toHaveCount(0)
  expect(await page.evaluate(()=>sessionStorage.length)).toBe(0)
})

test('AgentSpec explains insufficient access, missing source and storage failure without exposing error bodies', async ({page}) => {
  let status=403
  await serve(page, null, () => ({status, contentType:'text/plain', body:'private diagnostic'}))
  await address(page,'pages/overview/sections/agents/rows/scout/row_actions/open').click()
  await page.getByRole('button',{name:'View AgentSpec'}).click()
  const panel = page.getByRole('region',{name:'AgentSpec inspector'})
  await expect(panel).toContainText('Only a server operator')
  status=404
  await panel.getByRole('button',{name:'Retry'}).click()
  await expect(panel).toContainText('No AgentSpec source is available')
  status=503
  await panel.getByRole('button',{name:'Retry'}).click()
  await expect(panel).toContainText('The server could not read')
  status=401
  await panel.getByRole('button',{name:'Retry'}).click()
  await expect(panel).toContainText('Your viewer session has expired')
  await expect(panel).not.toContainText('private diagnostic')
  await expect(panel.locator('pre')).toHaveCount(0)
})

test('shipped operator profile shows its real routes, state transitions and allowed targets', async ({page}) => {
  const source=file('../../../examples/agents/operator.yaml')
  await serve(page,null,()=>({contentType:'application/yaml',body:source}))
  await page.goto('http://dashboard.test/?agent=scout&tab=spec')
  const profile=page.getByRole('tabpanel',{name:'Profile'})
  await expect(profile.getByRole('heading',{name:'Platform operator',exact:true})).toBeVisible()
  await expect(profile).toContainText('Read logs and metrics')
  const routing=profile.getByRole('region',{name:'Models and routing'})
  await expect(routing).toContainText('routes.yaml')
  await expect(routing).toContainText('example/Classifier')
  await expect(routing).toContainText('http://127.0.0.1:8080/v1')
  const flow=profile.getByRole('region',{name:'Turn flow'})
  await expect(flow).toContainText('operator-reply@1')
  await expect(flow).toContainText('0.5')
  const ask=flow.locator('.state').filter({has:page.getByText('Ask',{exact:true})})
  await expect(ask).toContainText('answer')
  await expect(ask).toContainText('reply.send')
  const bindings=profile.getByRole('region',{name:'Bindings'})
  await expect(bindings).toContainText('conversations.history')
  await expect(bindings).toContainText('/api/v1/query_range')
  await expect(profile.getByRole('region',{name:'A2A exposure'})).toContainText('/a2a')
  await expect(profile.getByRole('heading',{name:'Deployment'})).toBeVisible()
  await page.getByRole('tab',{name:'Profile'}).focus()
  await page.keyboard.press('ArrowRight')
  expect(await page.locator('#source-panel pre').textContent()).toBe(source)
  await page.keyboard.press('Home')
  await expect(profile).toBeVisible()
  await page.setViewportSize({width:375,height:812})
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true)
})
test('bounded YAML profile leaves exact source readable on parser refusal',async ({page})=>{
  const source='agent: &cycle {id: scout, next: *cycle}'
  await serve(page,null,()=>({contentType:'application/yaml',body:source}))
  await page.goto('http://dashboard.test/?agent=scout&tab=spec')
  await expect(page.getByRole('tabpanel',{name:'Profile'})).toContainText('cannot be displayed')
  await page.getByRole('tab',{name:'Source YAML'}).click()
  expect(await page.locator('#source-panel pre').textContent()).toBe(source)
})

test('send task selects only running A2A agents and retains the message ID after an uncertain delivery',async ({page})=>{
  const calls=[]
  await serve(page,null,null,(url,req)=>{
    expect(url.pathname).toBe('/api/agents/scout/dispatch')
    expect(req.method()).toBe('POST')
    expect(req.headers()['x-loom-agent-bearer']).toBe('agent-secret')
    calls.push(req.postDataJSON())
    return calls.length===1 ? {status:503,json:{error:'dispatch_unavailable'}} : {json:{run_id:'run-previous',state:'TASK_STATE_COMPLETED'}}
  })
  await page.getByRole('link',{name:'Dispatch',exact:true}).click()
  const pick=page.getByLabel('Running agent')
  await expect(pick).toHaveValue('scout')
  await expect(pick.locator('option')).toHaveText(['Select an agent','Scout · AgentSpec · Native'])
  await page.getByLabel('Task',{exact:true}).fill('Inspect the deployment')
  await page.getByLabel('Agent A2A bearer').fill('agent-secret')
  await page.getByRole('button',{name:'Send task'}).click()
  await expect(page.getByRole('alert')).toContainText('same message ID')
  await page.getByRole('button',{name:'Send task'}).click()
  await expect(page.getByRole('status')).toContainText('TASK_STATE_COMPLETED')
  expect(calls[0]).toEqual(calls[1])
  expect(calls[0].message_id).toMatch(/^[a-zA-Z0-9._-]{1,128}$/)
  expect(calls[0].text).toBe('Inspect the deployment')
  await expect(page.getByLabel('Agent A2A bearer')).toHaveValue('')
  expect(await page.evaluate(()=>JSON.stringify([location.href,sessionStorage,localStorage]))).not.toContain('agent-secret')
  await page.getByRole('link',{name:'View run'}).click()
  await expect(page.getByRole('heading',{name:'Run detail',exact:true})).toBeVisible()
})
test('launch matches the chosen harness to one of three templates and retries with one request ID',async ({page})=>{
  const commands=[]
  await serve(page,null,null,(url,req)=>{
    if(url.pathname==='/api/launch-templates') return {json:{templates:rows('templates')}}
    expect(url.pathname).toBe('/api/launch')
    expect(req.method()).toBe('POST')
    expect(req.headers()['x-loom-agent-bearer']).toBeUndefined()
    commands.push(req.postDataJSON())
    return commands.length===1 ? {status:503,json:{error:'launch_unavailable'}} : {status:201,json:{agent_id:'reviewer',instance:'review-1',status:'started',kind:'session',harness:'codex'}}
  })
  await page.getByRole('link',{name:'Dispatch',exact:true}).click()
  await page.getByRole('radio',{name:'Start agent'}).check()
  const harness=page.getByLabel('Harness')
  await expect(harness.locator('option')).toHaveText(['Select a harness','Native','Claude','Codex'])
  await expect(page.getByLabel('Agent template')).toHaveCount(0)
  for (const [choice,id,name] of [['native','scout','Scout · AgentSpec'],['claude','claude-review','Claude review · Session'],['codex','codex-review','Codex review · Session']]) {
    await harness.selectOption(choice)
    await expect(page.getByLabel('Agent template')).toHaveValue(id)
    await expect(page.getByLabel('Agent template').locator('option')).toHaveText(['Select a template',name])
  }
  await page.getByRole('button',{name:'Start agent'}).click()
  await expect(page.getByRole('alert')).toContainText('same request ID')
  await page.getByRole('button',{name:'Start agent'}).click()
  await expect(page.getByRole('status')).toContainText('Agent started')
  expect(commands[0]).toEqual(commands[1])
  expect(commands[0].harness).toBe('codex')
  expect(commands[0].template).toBe('codex-review')
  await page.getByRole('link',{name:'View agent'}).click()
  await expect(page.getByRole('heading',{name:'Reviewer',exact:true})).toBeVisible()
})

test('operator selects the recipient when two agents accept tasks',async ({page})=>{
  let sentTo
  const agents=fixture.agents.map(a=>a.id==='reviewer' ? {...a,a2a_protocol:'inbound-answer@1',kind:'session',harness:'codex'} : a)
  await serve(page,()=>({json:{...fixture,agents}}),null,(url,req)=>{
    sentTo=url.pathname
    expect(req.headers()['x-loom-agent-bearer']).toBe('recipient-token')
    return {json:{run_id:'review-result',state:'TASK_STATE_REJECTED'}}
  })
  await page.getByRole('link',{name:'Dispatch',exact:true}).click()
  const pick=page.getByLabel('Running agent')
  await expect(pick).toHaveValue('')
  await pick.selectOption('scout')
  await page.getByLabel('Agent A2A bearer').fill('previous-recipient')
  await pick.selectOption({label:'Reviewer · Session · Codex'})
  await expect(page.getByLabel('Agent A2A bearer')).toHaveValue('')
  await page.getByLabel('Task',{exact:true}).fill('Review changes')
  await page.getByLabel('Agent A2A bearer').fill('recipient-token')
  await page.getByRole('button',{name:'Send task'}).click()
  await expect(page.getByRole('status')).toContainText('TASK_STATE_REJECTED')
  expect(sentTo).toBe('/api/agents/reviewer/dispatch')
})

test('no dispatch targets, scoped viewer refusal and empty launch catalog',async ({page})=>{
  let denied=true
  await serve(page,()=>({json:{...fixture,agents:[]}}),null,()=>denied ? {status:403,json:{error:'forbidden'}} : {json:{templates:[]}})
  await page.getByRole('link',{name:'Dispatch',exact:true}).click()
  await expect(page.getByText('No running A2A agents are visible.')).toBeVisible()
  await expect(page.getByRole('button',{name:'Send task'})).toBeDisabled()
  await page.getByRole('radio',{name:'Start agent'}).check()
  await expect(page.getByRole('alert')).toContainText('server operator token')
  denied=false
  await page.getByRole('button',{name:'Retry'}).click()
  await expect(page.getByText('No launch templates are configured on this server.')).toBeVisible()
  await expect(page.getByRole('button',{name:'Start agent'})).toBeDisabled()
})
