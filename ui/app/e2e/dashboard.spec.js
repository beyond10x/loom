import {test, expect} from '@playwright/test'
import {readFileSync} from 'node:fs'
import {parse} from 'yaml'

function file(path) { return readFileSync(new URL(path, import.meta.url), 'utf8') }
function rows(name) { return JSON.parse(file('../../fixtures/' + name + '.json')).rows }
const html = file('../../dist/index.html')
const fixture = {...rows('snapshot')[0], agents:rows('agents'), runs:rows('runs'), stats:rows('statistics')[0]}
const scenarios = parse(file('../../tests/overview.yaml')).tests
const address = (page, at) => page.locator('[data-address="' + at + '"]')

async function serve(page, responder, specResponder) {
  await page.route('**/*', async route => {
    const url = new URL(route.request().url())
    expect(url.origin).toBe('http://dashboard.test')
    if (url.pathname === '/') return route.fulfill({body:html, contentType:'text/html'})
    if (url.pathname.startsWith('/api/agents/')) {
      expect(url.search).toBe('')
      expect(route.request().method()).toBe('GET')
      expect(route.request().headers().authorization).toBe('Bearer test-viewer')
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
    await serve(page)
    for (const step of scenario.steps) {
      if (step.open) await expect(page.getByRole('heading',{name:'Server overview'})).toBeVisible()
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
  const panel = page.getByRole('region',{name:'AgentSpec source'})
  await expect(panel).toContainText(spec.spec_revision)
  expect(await panel.locator('pre').textContent()).toBe(spec.source)
  expect(await panel.locator('markup').count()).toBe(0)
  expect(requests).toEqual(['/api/agents/scout/spec'])
  await page.getByRole('button',{name:'Hide AgentSpec'}).click()
  await expect(panel).toHaveCount(0)
  await view.click()
  await expect(panel).toContainText(spec.source.trim())
  await page.getByRole('link',{name:'Server overview'}).click()
  await address(page,'pages/overview/sections/agents/rows/reviewer/row_actions/open').click()
  await expect(view).toBeVisible()
  await expect(panel).toHaveCount(0)
  await page.goBack()
  await address(page,'pages/overview/sections/agents/rows/scout/row_actions/open').click()
  await view.click()
  await expect(panel).toContainText(spec.spec_revision)
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
  const panel = page.getByRole('region',{name:'AgentSpec source'})
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
