import {
  Body,
  Controller,
  Get,
  HttpCode,
  HttpStatus,
  Param,
  Post,
  Query,
  Req,
  UseGuards,
} from '@nestjs/common';
import { ApiHeader, ApiOperation, ApiResponse, ApiTags } from '@nestjs/swagger';
import { AssetDIDService } from './asset-did.service';
import { ApiKeyGuard } from '../auth/guards/api-key.guard';
import { AssetSyncRequest } from './dto/asset-sync-request.dto';
import { AssetSyncResponse } from './dto/asset-sync-response.dto';
import { AssetDIDDto, AssetDIDListQuery } from './dto/asset-did.dto';

/**
 * Machine-facing surface for asset and community DIDs.
 *
 * MOUNT POINT IS DELIBERATE (plan §4.3, risk R7). These routes live at `/asset-dids`, not
 * under `/did`, because `GET /did/:did` (`did.controller.ts:27-33`) would shadow any
 * nested asset route depending on module registration order in `app.module.ts`. Do not
 * "tidy" this under `/did`.
 *
 * `@UseGuards(ApiKeyGuard)` is at CLASS level, so a route added later is authenticated by
 * default rather than by remembering to decorate it. No human principal can reach these:
 * an asset can never obtain a JWT (plan §2.6), and `DIDOwnerGuard` returns true whenever
 * a route has no `:did` param (`did-owner.guard.ts:21-24`), i.e. it would allow everything
 * on `POST /asset-dids/sync`.
 */
@ApiTags('Asset DIDs')
@ApiHeader({
  name: 'x-api-key',
  required: true,
  description: 'Machine-to-machine API key (same value as the off-chain storage API_KEY)',
})
@ApiResponse({ status: HttpStatus.UNAUTHORIZED, description: 'invalid or missing x-api-key' })
@Controller('asset-dids')
@UseGuards(ApiKeyGuard)
export class AssetDIDController {
  constructor(private readonly assetDidService: AssetDIDService) {}

  @Post('sync')
  @HttpCode(HttpStatus.OK)
  @ApiOperation({
    summary: 'Bulk idempotent upsert of community and asset DIDs',
    description:
      'Re-posting an identical payload creates nothing and returns byte-identical DIDs. ' +
      'Subjects absent from the payload, within the scope it covers, are retired - never ' +
      'deleted, so DIDs referenced by already-issued certificates stay resolvable.',
  })
  @ApiResponse({ status: HttpStatus.OK, type: AssetSyncResponse })
  @ApiResponse({
    status: HttpStatus.BAD_REQUEST,
    description: 'Empty payload, duplicate subjectUuid, or an undeclared/malformed field',
  })
  @ApiResponse({
    status: HttpStatus.CONFLICT,
    description: 'A derived address is already held by a different subject; nothing was written',
  })
  async sync(@Body() syncRequest: AssetSyncRequest, @Req() req): Promise<AssetSyncResponse> {
    return this.assetDidService.syncSubjects(syncRequest, req);
  }

  // Declared before the `:subjectUuid` route so the intent is obvious at a glance; Nest
  // would not confuse them in any case, since this path has no trailing segment.
  @Get()
  @ApiOperation({ summary: 'List asset/community DID records, optionally filtered' })
  @ApiResponse({ status: HttpStatus.OK, type: [AssetDIDDto] })
  async list(@Query() query: AssetDIDListQuery): Promise<AssetDIDDto[]> {
    return this.assetDidService.list(query);
  }

  @Get(':subjectUuid')
  @ApiOperation({
    summary: 'Resolve one record by its canonical subject uuid (asset or community)',
  })
  @ApiResponse({ status: HttpStatus.OK, type: AssetDIDDto })
  @ApiResponse({ status: HttpStatus.NOT_FOUND, description: 'Subject never seen by a sync' })
  async findBySubjectUuid(@Param('subjectUuid') subjectUuid: string): Promise<AssetDIDDto> {
    return this.assetDidService.findBySubjectUuid(subjectUuid);
  }
}
