import { Injectable } from '@nestjs/common';
import { InjectModel } from '@nestjs/mongoose';
import { Model } from 'mongoose';
import { Request } from 'express';
import { AuditLog, AuditAction } from '../database/schemas';

@Injectable()
export class AuditService {
  constructor(
    @InjectModel(AuditLog.name) private auditLogModel: Model<AuditLog>,
  ) {}

  async log(
    action: AuditAction,
    did: string,
    request?: Request,
    metadata?: Record<string, any>,
    gsyDexAddress?: string,
    success = true,
  ): Promise<AuditLog> {
    const log = new this.auditLogModel({
      action,
      did,
      gsyDexAddress,
      metadata,
      success,
      ipAddress: request?.ip,
      userAgent: request?.headers['user-agent'],
    });

    return log.save();
  }

  async getLogsByDid(did: string): Promise<AuditLog[]> {
    return this.auditLogModel.find({ did }).sort({ createdAt: -1 }).exec();
  }
}