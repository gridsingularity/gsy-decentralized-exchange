import { Injectable, CanActivate, ExecutionContext, ForbiddenException } from '@nestjs/common';
import { Observable } from 'rxjs';

@Injectable()
export class DIDOwnerGuard implements CanActivate {
  canActivate(
    context: ExecutionContext,
  ): boolean | Promise<boolean> | Observable<boolean> {
    const request = context.switchToHttp().getRequest();
    const user = request.user;
    
    // If no user is authenticated, deny access
    if (!user) {
      return false;
    }

    // Get the DID from route parameters
    const params = request.params;
    const targetDid = params.did;

    // If no DID is specified in the route, allow access
    if (!targetDid) {
      return true;
    }

    // Check if the authenticated user's DID matches the requested DID
    const isOwner = user.did === targetDid;
    
    if (!isOwner) {
      throw new ForbiddenException('You do not have permission to access this resource');
    }
    
    return isOwner;
  }
}